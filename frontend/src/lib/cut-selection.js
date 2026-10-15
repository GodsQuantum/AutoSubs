// Timeline helpers shared by UI and node regression tests.
/** @typedef {{start:number,end:number}} KeepRange */
/** @param {number} seconds */
export const timeLabel = seconds => {
  const ms=Math.max(0,Math.round(seconds*1000)),h=Math.floor(ms/3600000),m=Math.floor(ms/60000)%60,s=Math.floor(ms/1000)%60;
  return [h,m,s].map(n=>String(n).padStart(2,'0')).join(':')+'.'+String(ms%1000).padStart(3,'0');
};

/** @param {KeepRange[]} ranges */
export const totalDuration = ranges => ranges.reduce((sum,r)=>sum+(r.end-r.start),0);

/** @param {KeepRange[]} existing @param {number} start @param {number} end @param {number} duration */
export function addRange(existing,start,end,duration) {
  if (!Number.isFinite(duration) || duration<=0 || !Number.isFinite(start) || !Number.isFinite(end)
    || start<0 || end>duration+.001 || end-start<.05) throw new Error('Plage ou durée invalide.');
  const next=[...existing,{start:Math.round(start*1000)/1000,end:Math.min(duration,Math.round(end*1000)/1000)}]
    .sort((a,b)=>a.start-b.start);
  if(next.length>32 || next.some((r,i)=>i>0 && r.start<next[i-1].end))
    throw new Error('Plages qui se chevauchent ou limite de 32 extraits.');
  return next;
}

/** @param {string|null} data @param {number} duration */
export function restoreRanges(data,duration) {
  if(!data || duration<=0) return [];
  try {
    const parsed=JSON.parse(data);
    if(!Array.isArray(parsed)||parsed.length>32) return [];
    return parsed.reduce((ranges,r)=>addRange(ranges,r.start,r.end,duration),[]);
  }catch{return [];}
}

/** @param {KeepRange[]} ranges @param {number} time */
export function nextPlaybackTime(ranges,time) {
  for(const r of ranges){
    if(time<r.start) return r.start;
    if(time<r.end-.035) return time;
  }
  return null;
}
