import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { Copy, Trash2, Check } from "lucide-react";
import { getHistory, clearHistory, HistoryEntry } from "../lib/tauri";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";

export default function History() {
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [copied, setCopied] = useState<string | null>(null);

  useEffect(() => {
    getHistory().then(setEntries);
  }, []);

  const handleCopy = async (entry: HistoryEntry) => {
    await writeText(entry.text);
    setCopied(entry.id);
    setTimeout(() => setCopied(null), 1500);
  };

  const handleClear = async () => {
    await clearHistory();
    setEntries([]);
  };

  return (
    <div className="flex flex-col h-screen bg-surface text-zinc-100">
      {/* Header */}
      <div className="flex items-center justify-between px-5 py-4 border-b border-border">
        <h1 className="text-base font-semibold">History</h1>
        {entries.length > 0 && (
          <button
            onClick={handleClear}
            className="flex items-center gap-1.5 text-xs text-zinc-500 hover:text-red-400
                       transition-colors"
          >
            <Trash2 size={13} />
            Clear all
          </button>
        )}
      </div>

      {/* List */}
      <div className="flex-1 overflow-y-auto p-4 space-y-2">
        {entries.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full gap-2 text-zinc-600">
            <div className="text-3xl">🎙</div>
            <div className="text-sm">No dictations yet</div>
          </div>
        ) : (
          entries.map((entry, i) => (
            <motion.div
              key={entry.id}
              initial={{ opacity: 0, y: 4 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: i * 0.02 }}
              className="group flex gap-3 p-3 rounded-xl bg-surface-raised border border-border
                         hover:border-zinc-600 transition-colors"
            >
              <div className="flex-1 min-w-0">
                <p className="text-sm text-zinc-100 leading-relaxed">{entry.text}</p>
                <div className="flex items-center gap-2 mt-1.5">
                  <span className="text-xs text-zinc-600">{entry.app_name}</span>
                  <span className="text-xs text-zinc-700">·</span>
                  <span className="text-xs text-zinc-600">
                    {new Date(entry.timestamp).toLocaleTimeString([], {
                      hour: "2-digit",
                      minute: "2-digit",
                    })}
                  </span>
                </div>
              </div>
              <button
                onClick={() => handleCopy(entry)}
                className="flex-shrink-0 opacity-0 group-hover:opacity-100 transition-opacity
                           text-zinc-500 hover:text-zinc-200 p-1"
              >
                {copied === entry.id ? (
                  <Check size={14} className="text-green-400" />
                ) : (
                  <Copy size={14} />
                )}
              </button>
            </motion.div>
          ))
        )}
      </div>
    </div>
  );
}
