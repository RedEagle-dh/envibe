import { useLayoutEffect, useRef, useState, useCallback, useMemo, memo } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { Terminal, Trash2, ArrowDownToLine, Pause, Search, X, MousePointer, Copy, Check } from 'lucide-react';
import { useStore, useServiceLogs, useSelectedService } from '../stores/useStore';
import type { LogEntry } from '../types';

const LOG_LINE_HEIGHT = 24;

export const LogViewer = memo(function LogViewer() {
  const selectedService = useSelectedService();
  const selectedServiceName = useStore((s) => s.selectedServiceName);
  const logs = useServiceLogs();
  const followLogs = useStore((s) => s.followLogs);
  const setFollowLogs = useStore((s) => s.setFollowLogs);
  const clearServiceLogs = useStore((s) => s.clearServiceLogs);

  const [filter, setFilter] = useState('');
  const [showSearch, setShowSearch] = useState(false);
  const [copied, setCopied] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  // Refs for stable callbacks — avoids stale closures and callback recreation
  const followRef = useRef(followLogs);
  followRef.current = followLogs;

  // Memoize search filter
  const filteredLogs = useMemo(() => {
    if (!filter) return logs;
    const lower = filter.toLowerCase();
    return logs.filter((log) => log.message.toLowerCase().includes(lower));
  }, [logs, filter]);

  const virtualizer = useVirtualizer({
    count: filteredLogs.length,
    getScrollElement: () => containerRef.current,
    estimateSize: () => LOG_LINE_HEIGHT,
    overscan: 20,
  });

  // Copy all visible logs to clipboard
  const copyLogs = useCallback(() => {
    const text = filteredLogs
      .map((log) => `${log.time} ${log.message}`)
      .join('\n');

    navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  }, [filteredLogs]);

  // Auto-scroll: useLayoutEffect runs before paint, preventing visual flash.
  // Uses virtualizer.scrollToIndex (pure math) instead of scrollIntoView (forces reflow).
  useLayoutEffect(() => {
    if (followRef.current && filteredLogs.length > 0) {
      virtualizer.scrollToIndex(filteredLogs.length - 1, { align: 'end' });
    }
  }, [filteredLogs.length, virtualizer]);

  // Throttled scroll detection via rAF — at most one check per frame.
  // Uses DOM measurements + ref for followLogs so the callback never needs recreation.
  const scrollRAF = useRef(0);
  const handleScroll = useCallback(() => {
    if (scrollRAF.current) return;
    scrollRAF.current = requestAnimationFrame(() => {
      scrollRAF.current = 0;
      if (!containerRef.current) return;
      const { scrollTop, scrollHeight, clientHeight } = containerRef.current;
      const isAtBottom = scrollHeight - scrollTop - clientHeight < 50;

      if (isAtBottom && !followRef.current) {
        setFollowLogs(true);
      } else if (!isAtBottom && followRef.current) {
        setFollowLogs(false);
      }
    });
  }, [setFollowLogs]);

  // Show placeholder when no service is selected
  if (!selectedServiceName) {
    return (
      <div className="panel h-full flex flex-col">
        <div className="panel-header">
          <span className="panel-title flex items-center gap-2">
            <Terminal size={16} className="text-envibe-accent" />
            Console
          </span>
        </div>
        <div className="panel-content flex-1 flex items-center justify-center">
          <div className="text-center text-envibe-text-muted">
            <MousePointer size={32} className="mx-auto mb-2 opacity-50" />
            <p className="text-sm">Select a service to view logs</p>
            <p className="text-xs mt-1 text-envibe-text-subtle">
              Click on a service in the Services panel
            </p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="panel h-full flex flex-col">
      <div className="panel-header">
        <span className="panel-title flex items-center gap-2">
          <Terminal size={16} className="text-envibe-accent" />
          {selectedServiceName}
          {selectedService && (
            <span className={`badge ${selectedService.status === 'running' ? 'badge-success' : 'badge-muted'}`}>
              {selectedService.status}
            </span>
          )}
        </span>
        <div className="flex items-center gap-1">
          {showSearch ? (
            <div className="flex items-center gap-1 bg-envibe-bg-tertiary rounded-md px-2">
              <Search size={14} className="text-envibe-text-muted" />
              <input
                type="text"
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
                placeholder="Filter logs..."
                className="bg-transparent border-none text-sm text-envibe-text placeholder:text-envibe-text-subtle focus:outline-none w-32"
                autoFocus
              />
              <button
                className="icon-btn p-0.5"
                onClick={() => {
                  setFilter('');
                  setShowSearch(false);
                }}
              >
                <X size={14} />
              </button>
            </div>
          ) : (
            <button
              className="icon-btn"
              onClick={() => setShowSearch(true)}
              title="Search logs"
            >
              <Search size={14} />
            </button>
          )}

          <button
            className={`icon-btn ${followLogs ? 'text-envibe-accent' : ''}`}
            onClick={() => setFollowLogs(!followLogs)}
            title={followLogs ? 'Stop following' : 'Follow logs'}
          >
            {followLogs ? <ArrowDownToLine size={14} /> : <Pause size={14} />}
          </button>

          <button
            className={`icon-btn ${copied ? 'text-envibe-success' : ''}`}
            onClick={copyLogs}
            title="Copy logs to clipboard"
          >
            {copied ? <Check size={14} /> : <Copy size={14} />}
          </button>

          <button
            className="icon-btn"
            onClick={() => selectedServiceName && clearServiceLogs(selectedServiceName)}
            title="Clear logs for this service"
          >
            <Trash2 size={14} />
          </button>
        </div>
      </div>

      <div
        ref={containerRef}
        className="panel-content flex-1 min-h-0 overflow-y-auto font-mono text-sm p-2"
        onScroll={handleScroll}
      >
        {filteredLogs.length === 0 ? (
          <div className="h-full flex items-center justify-center text-envibe-text-muted">
            <div className="text-center">
              <Terminal size={32} className="mx-auto mb-2 opacity-50" />
              <p>{filter ? 'No matching logs' : 'Waiting for logs...'}</p>
              <p className="text-xs mt-1 text-envibe-text-subtle">
                Start the service to see output
              </p>
            </div>
          </div>
        ) : (
          <div
            style={{
              height: virtualizer.getTotalSize(),
              width: '100%',
              position: 'relative',
            }}
          >
            {virtualizer.getVirtualItems().map((virtualRow) => (
              <div
                key={filteredLogs[virtualRow.index].id}
                style={{
                  position: 'absolute',
                  top: 0,
                  left: 0,
                  width: '100%',
                  height: virtualRow.size,
                  transform: `translateY(${virtualRow.start}px)`,
                }}
              >
                <LogLine log={filteredLogs[virtualRow.index]} />
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="px-3 py-2 border-t border-envibe-border text-xs text-envibe-text-subtle flex items-center justify-between">
        <span>
          {filteredLogs.length} {filteredLogs.length === 1 ? 'line' : 'lines'}
          {filter && ` (filtered from ${logs.length})`}
        </span>
        <span>
          {followLogs ? 'Following' : 'Paused'}
        </span>
      </div>
    </div>
  );
});

interface LogLineProps {
  log: LogEntry;
}

const LEVEL_COLORS: Record<LogEntry['level'], string> = {
  info: 'text-envibe-text',
  warn: 'text-envibe-warning',
  error: 'text-envibe-danger',
  debug: 'text-envibe-text-muted',
};

const LogLine = memo(function LogLine({ log }: LogLineProps) {
  return (
    <div className="flex gap-2 hover:bg-envibe-bg-tertiary/50 px-1 rounded">
      <span className="text-envibe-text-subtle flex-shrink-0 select-none">
        {log.time}
      </span>
      <span className={`${LEVEL_COLORS[log.level]} whitespace-pre`}>
        {log.message}
      </span>
    </div>
  );
});
