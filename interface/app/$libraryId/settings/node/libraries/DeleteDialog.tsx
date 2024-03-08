import { useLocale, usePlatform } from '@sd/interface-core';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from 'react-router';
import { useBridgeMutation, usePlausibleEvent, useZodForm } from '@sd/client';
import { Dialog, useDialog, UseDialogProps } from '@sd/ui';

interface Props extends UseDialogProps {
	libraryUuid: string;
}

export default function DeleteLibraryDialog(props: Props) {
	const { t } = useLocale();

	const submitPlausibleEvent = usePlausibleEvent();
	const queryClient = useQueryClient();
	const platform = usePlatform();
	const navigate = useNavigate();

	const deleteLib = useBridgeMutation('library.delete');

	const form = useZodForm();

	const onSubmit = form.handleSubmit(async () => {
		try {
			await deleteLib.mutateAsync(props.libraryUuid);

			queryClient.invalidateQueries(['library.list']);

			platform.refreshMenuBar && platform.refreshMenuBar();

			submitPlausibleEvent({
				event: {
					type: 'libraryDelete'
				}
			});

			navigate('/');
		} catch (e) {
			alert(`Failed to delete library: ${e}`);
		}
	});

	return (
		<Dialog
			form={form}
			onSubmit={onSubmit}
			dialog={useDialog(props)}
			title={t('delete_library')}
			description={t('delete_library_description')}
			ctaDanger
			ctaLabel={t('delete')}
		/>
	);
}
