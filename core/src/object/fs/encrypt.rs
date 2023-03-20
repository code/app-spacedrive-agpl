use crate::{job::*, library::Library};

use std::path::PathBuf;

use chrono::FixedOffset;
use sd_crypto::{
	crypto::Encryptor,
	header::FileHeader,
	primitives::{FILE_KEYSLOT_CONTEXT, LATEST_FILE_HEADER},
	types::{Algorithm, Key},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::fs::File;
use tracing::warn;

use super::{context_menu_fs_info, FsInfo, BYTES_EXT, ENCRYPTED_FILE_MAGIC_BYTES};

pub struct FileEncryptorJob;

#[derive(Serialize, Deserialize, Debug)]
pub struct FileEncryptorJobState {}

#[derive(Serialize, Deserialize, Type, Hash)]
pub struct FileEncryptorJobInit {
	pub location_id: i32,
	pub path_id: i32,
	pub key_uuid: uuid::Uuid,
	pub algorithm: Algorithm,
	pub metadata: bool,
	pub preview_media: bool,
	pub output_path: Option<PathBuf>,
}

#[derive(Serialize, Deserialize)]
pub struct Metadata {
	pub path_id: i32,
	pub name: String,
	pub hidden: bool,
	pub favorite: bool,
	pub important: bool,
	pub note: Option<String>,
	pub date_created: chrono::DateTime<FixedOffset>,
	pub date_modified: chrono::DateTime<FixedOffset>,
}

const JOB_NAME: &str = "file_encryptor";

#[async_trait::async_trait]
impl StatefulJob for FileEncryptorJob {
	type Init = FileEncryptorJobInit;
	type Data = FileEncryptorJobState;
	type Step = FsInfo;

	fn name(&self) -> &'static str {
		JOB_NAME
	}

	async fn init(&self, ctx: WorkerContext, state: &mut JobState<Self>) -> Result<(), JobError> {
		let step =
			context_menu_fs_info(&ctx.library.db, state.init.location_id, state.init.path_id)
				.await
				.map_err(|_| JobError::MissingData {
					value: String::from("file_path that matches both location id and path id"),
				})?;

		state.steps = [step].into_iter().collect();

		ctx.progress(vec![JobReportUpdate::TaskCount(state.steps.len())]);

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let info = &state.steps[0];

		let Library { key_manager, .. } = &ctx.library;

		if !info.path_data.is_dir {
			// handle overwriting checks, and making sure there's enough available space

			let user_key = key_manager
				.access_keymount(state.init.key_uuid)
				.await?
				.hashed_key;

			let user_key_details = key_manager.access_keystore(state.init.key_uuid).await?;

			let output_path = state.init.output_path.clone().map_or_else(
				|| {
					let mut path = info.fs_path.clone();
					let extension = path.extension().map_or_else(
						|| Ok("bytes".to_string()),
						|extension| {
							Ok::<String, JobError>(
								extension
									.to_str()
									.ok_or(JobError::MissingData {
										value: String::from(
											"path contents when converted to string",
										),
									})?
									.to_string() + BYTES_EXT,
							)
						},
					)?;

					path.set_extension(extension);
					Ok::<PathBuf, JobError>(path)
				},
				Ok,
			)?;

			let _guard = ctx
				.library
				.location_manager()
				.temporary_ignore_events_for_path(
					state.init.location_id,
					ctx.library.clone(),
					&output_path,
				)
				.await?;

			let mut reader = File::open(&info.fs_path).await?;
			let mut writer = File::create(output_path).await?;

			let master_key = Key::generate();

			let mut header = FileHeader::new(LATEST_FILE_HEADER, state.init.algorithm);

			header.add_keyslot(
				user_key_details.hashing_algorithm,
				user_key_details.content_salt,
				user_key,
				master_key.clone(),
				FILE_KEYSLOT_CONTEXT,
			)?;

			header
				.write_async(&mut writer, ENCRYPTED_FILE_MAGIC_BYTES)
				.await?;

			let encryptor = Encryptor::new(master_key, header.get_nonce(), state.init.algorithm)?;

			encryptor
				.encrypt_streams_async(&mut reader, &mut writer, header.get_aad().inner())
				.await?;
		} else {
			warn!(
				"encryption is skipping {} as it isn't a file",
				info.path_data.materialized_path
			)
		}

		ctx.progress(vec![JobReportUpdate::CompletedTaskCount(
			state.step_number + 1,
		)]);

		Ok(())
	}

	async fn finalize(&mut self, _ctx: WorkerContext, state: &mut JobState<Self>) -> JobResult {
		// mark job as successful
		Ok(Some(serde_json::to_value(&state.init)?))
	}
}
