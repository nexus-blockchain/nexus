# Nexus E2E

A clean-slate E2E test system built around two layers:

- Runtime contract checks: metadata-level ABI validation for pallets, calls, storage, and events.
- Smoke scenarios: a small set of signed flows that reflect the current runtime behavior.

Entry points:

- `npm run e2e`
- `npm run e2e:list`
- `npm run e2e:contracts`
- `npm run e2e:smoke`
- `npm run e2e:remote:list`
- `npm run e2e:remote:contracts`
- `npm run e2e:remote:inspect`
- `npm run e2e:remote:smoke:write`
- `npm run e2e:typecheck`

Remote commands default to `wss://202.140.140.202` and disable TLS certificate verification for the current process because the endpoint currently uses a self-signed certificate.

- `e2e:remote:contracts` and `e2e:remote:inspect` are read-only.
- `e2e:remote:smoke:write` submits transactions and mutates remote chain state.
