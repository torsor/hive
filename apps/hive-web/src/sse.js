/** Parse SSE frames from transcript stream (one JSON doc per `data:` line). */

export function parseSseDataPayload(raw) {
  const lines = String(raw).split(/\r?\n/);
  const dataLines = [];
  for (const line of lines) {
    if (line.startsWith("data:")) {
      dataLines.push(line.slice(5).trimStart());
    }
  }
  if (dataLines.length === 0) return null;
  const payload = dataLines.join("\n");
  if (!payload) return null;
  return JSON.parse(payload);
}

export function openTranscriptStream(url, { onDoc, onError }) {
  const source = new EventSource(url);
  source.addEventListener("tick", (ev) => {
    try {
      const doc = JSON.parse(ev.data);
      onDoc(doc);
    } catch (e) {
      onError(e);
    }
  });
  source.onerror = (ev) => {
    onError(ev);
    source.close();
  };
  return () => source.close();
}
