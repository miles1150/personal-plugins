import React from 'react';
export function Notice({children,tone='neutral',urgent=false}) { return <div className={'notice '+(tone==='neutral'?'':tone)} role={urgent?'alert':undefined}>{children}</div>; }
