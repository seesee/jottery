/**
 * Markup that has defeated sanitisers, or would defeat a naive one. Shared by
 * htmlSanitiser.test.ts and e2e/preview-security.spec.ts, because only a real
 * browser parses the mutation cases the way an attacker relies on.
 */
export const HOSTILE_HTML = [
  // Direct
  '<img src=x onerror=window.__pwned=1>',
  '<svg onload=window.__pwned=1>',
  '<svg><script>window.__pwned=1</script></svg>',
  '<script>window.__pwned=1</script>',
  '<body onload=window.__pwned=1>',
  '<details open ontoggle=window.__pwned=1>x</details>',
  '<input autofocus onfocus=window.__pwned=1>',
  '<video src=x onerror=window.__pwned=1></video>',
  '<audio src=x onerror=window.__pwned=1></audio>',
  '<a href="note:x" onclick="window.__pwned=1">x</a>',

  // Containers and active embeds
  '<iframe srcdoc="<script>parent.__pwned=1</script>"></iframe>',
  '<object data="javascript:window.__pwned=1"></object>',
  '<embed src="javascript:window.__pwned=1">',
  '<template><img src=x onerror=window.__pwned=1></template>',
  '<meta http-equiv="refresh" content="0;url=javascript:window.__pwned=1">',
  '<base href="javascript:/a/">',
  '<style>@import "javascript:window.__pwned=1";</style>',
  '<form action="javascript:window.__pwned=1"><button>x</button></form>',

  // Text that a second parse would read as markup
  '<xmp><img src=x onerror=window.__pwned=1></xmp>',
  '<textarea><img src=x onerror=window.__pwned=1></textarea>',
  '<title><img src=x onerror=window.__pwned=1></title>',
  '<noembed><img src=x onerror=window.__pwned=1></noembed>',
  '<noframes><img src=x onerror=window.__pwned=1></noframes>',
  '<plaintext><img src=x onerror=window.__pwned=1>',
  '<!--><img src=x onerror=window.__pwned=1>-->',
  '<![CDATA[<img src=x onerror=window.__pwned=1>]]>',
  '<?xml version="1.0"?><img src=x onerror=window.__pwned=1>',

  // Mutation XSS: namespace confusion and raw text
  '<noscript><p title="</noscript><img src=x onerror=window.__pwned=1>"></noscript>',
  '<svg></p><style><a id="</style><img src=1 onerror=window.__pwned=1>">',
  '<math><mtext><table><mglyph><style><img src=1 onerror=window.__pwned=1>',
  '<math><mtext><table><mglyph><style><!--</style><img title="--&gt;&lt;img src=1 onerror=window.__pwned=1&gt;">',
  '<select><template><style><!--</style><a rel="--><img src=x onerror=window.__pwned=1>">',
  '<form><math><mtext></form><form><mglyph><style></math><img src=x onerror=window.__pwned=1>',
  '<svg><desc><math><mi><style><img src=x onerror=window.__pwned=1>',
  '<table><svg><foreignObject><p><style><img src=x onerror=window.__pwned=1>',

  // Attributes
  '<p title="&quot;&gt;&lt;img src=x onerror=window.__pwned=1&gt;">x</p>',
  '<a href="javascript:window.__pwned=1">x</a>',
  '<a href="java&#9;script:window.__pwned=1">x</a>',
  '<a href="&#1;javascript:window.__pwned=1">x</a>',
  '<img src="data:text/html,<script>window.__pwned=1</script>">',
  '<a href="data:text/html,<script>window.__pwned=1</script>">x</a>',
];

/** What must never be left in the output, whatever went in. */
export const FORBIDDEN_ELEMENTS = 'script, style, iframe, object, embed, form, svg, math, template, meta, base, link, video, audio, noscript, xmp, textarea, title';
