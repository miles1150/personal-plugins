import React from 'react';
export function Field({id,label,value,error,onChange}) { return <label htmlFor={id}>{label}<input id={id} value={value} onChange={onChange} aria-invalid={!!error} aria-describedby={error?id+'-error':undefined}/>{error&&<span id={id+'-error'} role="status" style={{color:'var(--danger)',fontSize:'var(--type-xs)'}}>{error}</span>}</label>; }
