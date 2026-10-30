import React from 'react';
export function SpacePicker({spaces,selected,onChange}) {return <label>当前 Space<select value={selected} onChange={onChange}>{spaces.map(s=><option key={s}>{s}</option>)}</select><span className="small muted">切换工作台不会变更已有助手授权。</span></label>;}
