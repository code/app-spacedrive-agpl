import { useIsDark } from '@sd/interface-core';
import clsx from 'clsx';

import classes from './Thumb.module.scss';

export const useFrame = () => {
	const isDark = useIsDark();

	const className = clsx(
		'rounded-sm border-2 border-app-line bg-app-darkBox',
		isDark ? classes.checkers : classes.checkersLight
	);

	return { className };
};
