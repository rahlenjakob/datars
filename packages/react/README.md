# @datars/react

React bindings for [datars](../../README.md): `<DatarsView>` wraps the `<datars-view>` element.

```tsx
import { useRef } from "react";
import { DatarsView, useDatarsState, type DatarsViewHandle } from "@datars/react";
import votes from "./votes.chart.ts"; // compiled by @datars/vite or @datars/next

export function Votes() {
  const ref = useRef<DatarsViewHandle>(null);
  const s = useDatarsState(ref);
  return (
    <>
      <DatarsView ref={ref} chart={votes} mode="dark" />
      {(s?.states ?? votes.states ?? []).map((name) => (
        <button key={name} aria-pressed={s?.state === name} onClick={() => ref.current?.goto(name)}>{name}</button>
      ))}
    </>
  );
}
```

- **SSR-safe.** On the server (and in the first client render) it's a box at the chart's aspect —
  from the document's size, or `height` / `aspectRatio` — so nothing moves when the chart arrives.
  The runtime (`@datars/web`) is a lazy chunk loaded after hydration; nothing touches `window`
  during render. `"use client"` is included: server components can render it.
- **Props:** `chart` (a chart import or a document object), `document`, `src`, `doc`, `state`,
  `mode`, `height`, `aspectRatio`, `signals`, `tokens`, `data`, `lazy`, `label`, `attributes`,
  `runtime`, `onStateChange`, `onReady`, `onError`, plus any `div` attribute for the box.
- **Ref:** `goto`, `next`, `prev`, `send`, `setSignal`, `setTokens`, `setData`, `element`, `status`,
  `subscribe`. **Hook:** `useDatarsState(ref)`.
- Mounts near the viewport and unmounts far away (`lazy`, default on), keeping the reader's step.
- React 18 and 19. ESM with types.

Setup with Vite or Next.js, publish mode, HMR and how the runtime is served:
[Using datars with React](../../docs/guides/react.md).
