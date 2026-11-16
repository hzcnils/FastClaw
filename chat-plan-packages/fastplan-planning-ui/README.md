# FastPlan Planning UI

Reusable Vue 3 planning, confirmation, execution and node-editing UI used by FastPlan.

```js
import { FastPlanPlanningPanel, FastPlanExecutionDagPanel } from 'planflow-planning-ui'
import 'planflow-planning-ui/style.css'
```

`FastPlanPlanningPanel` receives the runtime state through `state` and emits presentation-neutral events for confirm, cancel, replan, file input and human responses. HTTP, WebSocket, SSE, restore and upload behavior is provided by `planflow-runtime`.

The backend contract is the FastPlan API itself: consumers inject `apiBase` and `adminBase`, then reuse the runtime client and transports. The package does not read host environment variables. Session-list UI is intentionally not included.

The package owns its font-size baseline through `--jcr-font-size`; FastPlan host `text-xs` utilities do not reduce planning-card text when the panel wrapper is used.
