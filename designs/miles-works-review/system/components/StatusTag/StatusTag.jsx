import React from 'react';
export function StatusTag({label,tone='neutral'}) { return <span className={'tag '+(tone==='neutral'?'':tone)}>{label}</span>; }
