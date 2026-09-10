// A fetch that answers from a script instead of the network.

/**
 * @param {(url: string, init?: RequestInit) => {status?: number, body?: unknown} | Error} script
 * @returns {typeof fetch & {calls: Array<{url: string, init?: RequestInit}>}}
 */
export function fakeFetch(script) {
  const calls = [];
  const f = async (url, init) => {
    calls.push({ url, init });
    const answer = script(url, init);
    if (answer instanceof Error) throw answer;
    const status = answer.status ?? 200;
    return {
      ok: status >= 200 && status < 300,
      status,
      statusText: status === 404 ? "Not Found" : "",
      async json() {
        if (answer.body === undefined) throw new SyntaxError("empty body");
        return answer.body;
      },
    };
  };
  f.calls = calls;
  return f;
}
