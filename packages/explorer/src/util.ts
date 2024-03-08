import { useZodSearchParams } from '@sd/interface-core';
import { useMemo } from 'react';
import { ExplorerItem, getExplorerItemData, useSelector } from '@sd/client';
import { z } from '@sd/ui';

import { explorerStore, flattenThumbnailKey } from './store';

export const PathParamsSchema = z.object({ path: z.string().optional() });
export const ExplorerParamsSchema = PathParamsSchema.extend({
	take: z.coerce.number().default(100)
});
export type ExplorerParams = z.infer<typeof ExplorerParamsSchema>;

export function useExplorerSearchParams() {
	return useZodSearchParams(ExplorerParamsSchema);
}

export function useExplorerItemData(explorerItem: ExplorerItem) {
	const newThumbnail = useSelector(explorerStore, (s) => {
		const firstThumbnail =
			explorerItem.type === 'Label'
				? explorerItem.thumbnails?.[0]
				: 'thumbnail' in explorerItem && explorerItem.thumbnail;

		return !!(firstThumbnail && s.newThumbnails.has(flattenThumbnailKey(firstThumbnail)));
	});

	return useMemo(() => {
		const itemData = getExplorerItemData(explorerItem);

		if (!itemData.hasLocalThumbnail) {
			itemData.hasLocalThumbnail = newThumbnail;
		}

		return itemData;
	}, [explorerItem, newThumbnail]);
}

export type ExplorerItemData = ReturnType<typeof useExplorerItemData>;

export const pubIdToString = (pub_id: number[]) =>
	pub_id.map((b) => b.toString(16).padStart(2, '0')).join('');

export const uniqueId = (item: ExplorerItem | { pub_id: number[] }) => {
	if ('pub_id' in item) return pubIdToString(item.pub_id);

	const { type } = item;

	switch (type) {
		case 'NonIndexedPath':
			return item.item.path;
		case 'SpacedropPeer':
		case 'Label':
			return item.item.name;
		default:
			return pubIdToString(item.item.pub_id);
	}
};

export function getItemId(index: number, items: ExplorerItem[]) {
	const item = items[index];
	return item ? uniqueId(item) : undefined;
}

export function getItemData(index: number, items: ExplorerItem[]) {
	return items[index];
}
