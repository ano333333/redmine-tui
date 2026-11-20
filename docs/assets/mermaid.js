// <pre class="mermaid"> を図として描画する。
// file:// で開いても動くよう、module script ではなく通常の script から動的 import する。
import("https://cdn.jsdelivr.net/npm/mermaid@11.16.1/+esm").then(({ default: mermaid }) => {
  mermaid.initialize({ startOnLoad: false, securityLevel: "loose", theme: "dark" });
  mermaid.run({ querySelector: "pre.mermaid" });
});
