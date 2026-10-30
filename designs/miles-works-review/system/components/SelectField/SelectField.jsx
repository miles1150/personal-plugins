import React from 'react';
export function SelectField({id,label,options,value,onChange}) { return <label htmlFor={id}>{label}<select id={id} value={value} onChange={onChange}>{options.map(o=><option key={o}>{o}</option>)}</select></label>; }
