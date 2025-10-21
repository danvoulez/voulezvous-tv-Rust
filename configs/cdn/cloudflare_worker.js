export default {
  async fetch(request) {
    const url = new URL(request.url);
    if (url.pathname.endsWith(".m3u8")) {
      url.hostname = "origin.voulezvous.ts.net";
    }
    return fetch(url.toString(), request);
  }
};
