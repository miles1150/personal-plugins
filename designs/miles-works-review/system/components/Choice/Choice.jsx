import React from 'react';
export function Choice({label,description,checked,disabled=false,onChange}) { return <label className="check"><input type="checkbox" checked={checked} disabled={disabled} onChange={onChange}/><span>{label}{description&&<p>{description}</p>}</span></label>; }
