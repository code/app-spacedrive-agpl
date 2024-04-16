import { useNavigation } from '@react-navigation/native';
import { forwardRef, useState } from 'react';
import { Text, View } from 'react-native';
import { useLibraryMutation } from '@sd/client';
import { Modal, ModalRef } from '~/components/layout/Modal';
import { Button } from '~/components/primitive/Button';
import { ModalInput } from '~/components/primitive/Input';
import { tw } from '~/lib/tailwind';
import { useSearchStore } from '~/stores/searchStore';

const SaveSearchModal = forwardRef<ModalRef>((_, ref) => {
	const [searchName, setSearchName] = useState('');
	const navigation = useNavigation();
	const searchStore = useSearchStore();
	const saveSearch = useLibraryMutation('search.saved.create');

	return (
		<Modal snapPoints={['22']} title="Save search" ref={ref}>
			<View style={tw`p-4`}>
				<ModalInput
					autoFocus
					value={searchName}
					onChangeText={(text) => setSearchName(text)}
					placeholder="Search Name..."
				/>
				<Button
					disabled={searchName.length === 0}
					style={tw`mt-2`}
					variant="accent"
					onPress={() => {
						saveSearch.mutate(
							{
								name: searchName,
								filters: JSON.stringify(searchStore.mergedFilters),
								description: null,
								icon: null,
								search: null
							},
							{
								onSuccess: () => {
									navigation.navigate('SearchStack', {
										screen: 'Search'
									});
								}
							}
						);
						setSearchName('');
					}}
				>
					<Text style={tw`font-medium text-ink`}>Save</Text>
				</Button>
			</View>
		</Modal>
	);
});

export default SaveSearchModal;