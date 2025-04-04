import React from 'react';
import { SvgIcon, SvgIconProps } from '@mui/material';

export default function IconModule(props: SvgIconProps) {
    return (
        <SvgIcon
            fontSize="inherit"
            {...props}>
            {/* tslint:disable-next-line: max-line-length */}
            <svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg" fill="currentColor">
                <path d="M13.85 4.44l-3.28-3.3-.35-.14H2.5l-.5.5v13l.5.5h11l.5-.5V4.8l-.15-.36zM13 5h-3V2l3 3zM3 14V2h6v3.5l.5.5H13v8H3z" />
            </svg>
        </SvgIcon>
    );
}
