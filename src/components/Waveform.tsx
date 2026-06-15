import { useEffect, useRef } from "react";

const BAR_COUNT = 42;

interface WaveformProps {
  active: boolean;
}

export default function Waveform({ active }: WaveformProps) {
  const levels = useRef<number[]>(new Array(BAR_COUNT).fill(0));
  const barsRef = useRef<(HTMLDivElement | null)[]>([]);

  useEffect(() => {
    if (!active) {
      levels.current.fill(0);
      barsRef.current.forEach((bar) => {
        if (bar) bar.style.height = "3px";
      });
    }
  }, [active]);

  // Called by parent via a forwarded ref or context
  // We expose a push function via the ref trick
  (Waveform as any)._push = (rms: number) => {
    levels.current.shift();
    levels.current.push(rms);
    barsRef.current.forEach((bar, i) => {
      if (!bar) return;
      const h = Math.max(3, Math.min(36, levels.current[i] * 200));
      bar.style.height = `${h}px`;
    });
  };

  return (
    <div className="flex items-center justify-center gap-[2px] h-full">
      {Array.from({ length: BAR_COUNT }, (_, i) => (
        <div
          key={i}
          ref={(el) => { barsRef.current[i] = el; }}
          className="w-[3px] rounded-full bg-accent transition-[height] duration-75"
          style={{ height: "3px", opacity: 0.85 }}
        />
      ))}
    </div>
  );
}
