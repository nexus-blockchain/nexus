# Nexus E2E

A clean-slate E2E test system built around two layers:

- Runtime contract checks: metadata-level ABI validation for pallets, calls, storage, and events.
- Smoke scenarios: a small set of signed flows that reflect the current runtime behavior.

Entry points:

- `npm run e2e`
- `npm run e2e:list`
- `npm run e2e:contracts`
- `npm run e2e:smoke`
- `npm run e2e:typecheck`
