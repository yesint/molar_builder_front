import { API_BASE } from "./config.js";

export const apiBase = API_BASE.replace(/\/$/, "");

async function fetchApi(path, options = {}) {
  const response = await fetch(`${apiBase}${path}`, {
    signal: AbortSignal.timeout(60000),
    ...options,
    redirect: "error",
    credentials: "omit",
    cache: "no-store",
  });
  if (!response.ok) {
    const data = await response.json().catch(() => null);
    const error = new Error(data?.error || `The server returned HTTP ${response.status}.`);
    error.status = response.status;
    throw error;
  }
  return response;
}

export async function request(path, options = {}) {
  const response = await fetchApi(path, options);
  const data = await response.json().catch(() => null);
  if (!data) throw new Error("The server did not return a valid response.");
  return data;
}

const PROGRESS_TYPE = "application/vnd.param-gen.stream";

async function readProgress(response, onProgress) {
  const reader = response.body.getReader();
  const decoder = new TextDecoder("utf-8", { fatal: true });
  let pending = new Uint8Array();
  let metadata;
  let completed = -1;
  let total;
  let bytes = 0;
  const parts = [];
  const invalid = () => new Error("The ZIP transfer was incomplete or damaged.");
  const append = data => {
    bytes += data.byteLength;
    if (bytes > metadata.bytes) throw invalid();
    parts.push(data);
  };
  try {
    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      if (metadata) {
        append(value);
        continue;
      }
      const buffer = new Uint8Array(pending.length + value.length);
      buffer.set(pending);
      buffer.set(value, pending.length);
      pending = buffer;
      let end;
      while (!metadata && (end = pending.indexOf(10)) >= 0) {
        if (end > 8192) throw invalid();
        let event;
        try { event = JSON.parse(decoder.decode(pending.subarray(0, end))); }
        catch { throw invalid(); }
        pending = pending.subarray(end + 1);
        if (event?.type === "error") {
          const error = new Error(event.error || "Computation failed.");
          error.status = event.status;
          throw error;
        }
        if (event?.type === "progress") {
          if (!Number.isSafeInteger(event.total) || event.total < 1 || event.total > 1000 ||
              !Number.isSafeInteger(event.completed) || event.completed !== completed + 1 ||
              event.completed > event.total || (total !== undefined && total !== event.total)) throw invalid();
          completed = event.completed;
          total = event.total;
          onProgress?.(completed, total);
        } else if (event?.type === "result") {
          if (!Number.isSafeInteger(event.bytes) || event.bytes <= 0 ||
              !/^[0-9a-f]{64}$/.test(event.sha256) || !["complete", "partial"].includes(event.status)) throw invalid();
          metadata = event;
          if (pending.length) append(pending);
          pending = new Uint8Array();
        } else throw invalid();
      }
      if (!metadata && pending.length > 8192) throw invalid();
    }
    if (!metadata || bytes !== metadata.bytes) throw invalid();
    return { blob: new Blob(parts, { type: "application/zip" }), metadata };
  } catch (error) {
    await reader.cancel().catch(() => {});
    throw error;
  } finally {
    reader.releaseLock();
  }
}

export async function compute(file, paymentToken, timeoutSeconds, onProgress) {
  const response = await fetchApi("/compute", {
    method: "POST",
    headers: { "Content-Type": "chemical/x-mdl-sdfile", Authorization: `Bearer ${paymentToken}`, Accept: PROGRESS_TYPE },
    body: file,
    signal: AbortSignal.timeout((timeoutSeconds + 120) * 1000),
  });
  // Older deployments can still return a plain ZIP until they are updated.
  const { blob, metadata } = response.headers.get("Content-Type")?.split(";")[0] === PROGRESS_TYPE
    ? await readProgress(response, onProgress)
    : { blob: await response.blob(), metadata: {
      bytes: Number(response.headers.get("X-Result-Bytes")), sha256: response.headers.get("X-Result-SHA256"),
      status: response.headers.get("X-Result-Status"),
    } };
  const digest = await crypto.subtle.digest("SHA-256", await blob.arrayBuffer());
  const sha256 = Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, "0")).join("");
  if (blob.type !== "application/zip" || blob.size !== metadata.bytes ||
      sha256 !== metadata.sha256 || !["complete", "partial"].includes(metadata.status)) {
    throw new Error("The ZIP transfer was incomplete or damaged.");
  }
  return { blob, status: metadata.status };
}
