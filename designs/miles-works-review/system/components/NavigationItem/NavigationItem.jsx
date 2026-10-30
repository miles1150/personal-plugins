import React from 'react';
export function NavigationItem({label,current=false,onClick}) { return <button type="button" className={current?'secondary':'ghost'} aria-current={current?'page':undefined} onClick={onClick}>{label}</button>; }
