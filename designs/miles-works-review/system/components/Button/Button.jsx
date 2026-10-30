import React from 'react';
export function Button({label, variant='primary', disabled=false, onClick}) { return <button type="button" className={variant==='primary'?'':variant} disabled={disabled} onClick={onClick}>{label}</button>; }
