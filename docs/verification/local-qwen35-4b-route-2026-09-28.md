# Local Qwen3.5-4B Route Verification

Date: 2026-09-28

## Scope

This is a loopback verification of a local LM Studio model behind Jev-Switch. It
does not claim that the ordinary Qwen3.5-4B checkpoint is a FastJev, Kev, or Jev
checkpoint.

## Upstream

- Provider id: `qwen35`
- Kind: `typesafe`
- Base: `http://127.0.0.1:8781/v1/systemone`
- Upstream model: `qwen35-4b-local`
- Backend: LM Studio, `Qwen_Qwen3.5-4B-Q4_K_M_3.gguf`
- Artifact size: `3,013,027,808` bytes
- Artifact SHA-256: `13c16f426047E2DE38CD075BDADE4A7BCBC8C774384876F677740CDA65F8A983`

The bridge returns structured Choice/Score/Noul-compatible JSON. Its probability
field is a model-generated bridge field and is not a calibrated Jev probability.

## Public route setup

An important runtime distinction was reproduced:

1. `PUT /v1/admin/providers` can register the provider.
2. `PUT /v1/admin/routes` can persist an edge such as `qwen35-local -> qwen35`.
3. The public `GET /v1/models` handler publishes only enabled service endpoints.

Therefore steps 1 and 2 alone leave `qwen35-local` absent from `/v1/models` and
`POST /v1/systemone` returns 404. Creating this service endpoint makes the route
public and hot-reloadable:

```json
{
  "id": "qwen35-local",
  "strategy_config": {"type": "follow_global"},
  "routes": [
    {
      "left": "qwen35-local",
      "match": "exact",
      "right": "qwen35",
      "upstream_model": "qwen35-4b-local",
      "priority": 40,
      "sticky": "none",
      "on_error": "next"
    }
  ]
}
```

After creation, `/v1/models` advertised `qwen35-local` and a smoke request
returned `route_trace.selected_provider = "qwen35"`,
`selected_model = "qwen35-4b-local"`, and `upstream_calls = 1`.

## Benchmark evidence

The Jev Decision Lab canonical 80-row life-record run through
`http://127.0.0.1:11439/v1/systemone` produced:

- 80/80 successful requests
- human-label agreement: `39/80 = 48.75%`
- mean end-to-end latency: `2,234.3 ms`
- P50 latency: `2,199.2 ms`
- direct bridge and gateway choices matched

This is useful protocol and integration evidence, not evidence that a general
Qwen checkpoint has Jev-level calibrated decision quality. The latency is far
outside the current Laya baseline (`45.6 ms` mean on the same life-record
workload).

## Follow-up

The provider/route/service-endpoint consistency rule should become an automated
daemon acceptance test. A configuration that is durable in SQLite but absent
from `/v1/models` is not a usable downstream integration.

## Fractional latency compatibility

Kev's System One response reports elapsed milliseconds as a fractional JSON
number, for example `162.4`. The protocol layer previously parsed
`JevResponse.latency_ms` as an integer and returned 502 before the answer could
reach the caller. The fix accepts integer or fractional milliseconds and rounds
at the protocol boundary; serialization remains an integer. `cargo test -p
jev-protocol` passes 25/25 tests after the change.
