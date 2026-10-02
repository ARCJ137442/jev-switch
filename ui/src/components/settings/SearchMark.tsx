import { highlightSegments } from './settingsSearch';

export function SearchMark({ text, query }: { text: string; query: string }) {
  return <>{highlightSegments(text, query).map((segment, index) => segment.matched
    ? <mark key={index} style={{ background: 'var(--accent)', color: '#fff', borderRadius: '0.15rem' }}>{segment.text}</mark>
    : segment.text)}</>;
}
