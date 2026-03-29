# Platform Coverage (Legal-Safe)

This document tracks Xenobot's current legal-safe platform adapter coverage.

## Scope

- Source discovery in user-accessible local directories.
- Authorized export parsing path only.
- No process-memory key extraction.
- No encryption bypass.

Explicitly excluded implementation styles:

- DLL hook chains
- process-memory scanning
- Windows-only key extraction helpers
- reference-project UI/asset reuse

## Machine-Readable Truth Source

The canonical machine-readable view now lives in:

- `GET /core/platform-capabilities`

As of the current backend audit:

- platforms listed in the matrix: `17`
- platforms at the current WeChat reference depth: `1`
- platforms below the current WeChat reference depth: `16`
- platforms with a platform-specific runtime detector layer: `1`
- platforms with a platform-specific legal-safe decrypt path: `1`
- platforms at the full planned end state: `0`

Important interpretation rule:

- shared downstream analysis/API availability means a platform's records can be queried after successful normalized import
- it does **not** mean that the platform's native export/runtime workflow is already complete to the full target depth

## Covered Platforms (17)

1. WeChat (`xenobot-wechat`)
2. WhatsApp (`xenobot-whatsapp`)
3. LINE (`xenobot-line`)
4. QQ (`xenobot-qq`)
5. Telegram (`xenobot-telegram`)
6. Discord (`xenobot-discord`)
7. Instagram (`xenobot-instagram`)
8. iMessage (`xenobot-imessage`)
9. Messenger (`xenobot-messenger`)
10. KakaoTalk (`xenobot-kakaotalk`)
11. Slack (`xenobot-slack`)
12. Teams (`xenobot-teams`)
13. Signal (`xenobot-signal`)
14. Skype (`xenobot-skype`)
15. Google Chat (`xenobot-googlechat`)
16. Zoom (`xenobot-zoom`)
17. Viber (`xenobot-viber`)

## Current Implementation Depth

Xenobot tracks platform work in four practical tiers:

- `Tier A`: deep platform workflow coverage beyond parsing, such as platform-specific runtime config, service orchestration, media/decrypt/monitor modules, and broader integration hooks.
- `Tier B+`: non-reference platforms that already moved beyond the shared skeleton and now expose richer service orchestration entrypoints approaching the WeChat reference surface.
- `Tier B`: legal-safe service skeleton coverage, including adapter + runtime config + authorized-root guard + staged export orchestration + crate tests.
- `Tier C`: baseline legal-safe adapter coverage, including source discovery + authorized export parsing + crate tests.

Current status:

- `Tier A`
  - WeChat (`xenobot-wechat`) with the current reference module set:
    - `account`
    - `audio`
    - `decrypt`
    - `media`
    - `monitor`
    - `service`
    - explicit account-view discovery
    - explicit media-inventory service entrypoint
    - explicit export-monitor service entrypoint
    - service-level decrypt authorization on both input and derived output paths
    - eager PID-scoped decrypt output directory creation before file writes
    - runtime detector/key fallback/cache/event regressions
- `Tier B+`
  - WhatsApp (`xenobot-whatsapp`)
  - LINE (`xenobot-line`)
  - QQ (`xenobot-qq`)
  - Telegram (`xenobot-telegram`)
  - Discord (`xenobot-discord`)
  - Instagram (`xenobot-instagram`)
  - iMessage (`xenobot-imessage`)
  - Messenger (`xenobot-messenger`)
  - KakaoTalk (`xenobot-kakaotalk`)
  - Slack (`xenobot-slack`)
  - Teams (`xenobot-teams`)
  - Signal (`xenobot-signal`)
  - Skype (`xenobot-skype`)
  - Google Chat (`xenobot-googlechat`)
  - Zoom (`xenobot-zoom`)
  - Viber (`xenobot-viber`)

These `Tier B+` platforms now add the next-stage orchestration layer on top of the shared module set:

- aggregated authorized workspace assembly
- optional monitor preparation bundled with workspace preparation
- service-level audio transcoding entrypoints

All sixteen non-WeChat platforms now share the same next-parity legal-safe module set:

- `account`
- `audio`
- `media`
- `monitor`
- `service`

This means Xenobot already covers all 17 target platforms at the legal-safe adapter layer, all 16 non-WeChat platforms have completed the same second-stage deepening pass, and all 16 non-WeChat platforms have now entered the third-stage orchestration pass.

The current highest-priority parity subset remains:

- WeChat (`xenobot-wechat`)
- WhatsApp (`xenobot-whatsapp`)
- Telegram (`xenobot-telegram`)
- Discord (`xenobot-discord`)
- LINE (`xenobot-line`)
- QQ (`xenobot-qq`)
- Instagram (`xenobot-instagram`)

For this subset, Xenobot now also enforces a tighter service-level regression layer:

- workspace account views must stay aligned with `discover_accounts()` / `primary_account()`
- `prepare_authorized_workspace(...)` must reject an unauthorized watch directory
- runtime service behavior is covered for WeChat key fallback, detector success/failure paths, and start/stop state transitions
- runtime service behavior is also covered for WeChat decrypt authorization boundaries and decrypt failure event emission
- runtime authorization mutation must take effect after `add_authorized_root(...)`
- runtime authorization mutation must allow authorized workspace construction immediately after the root is added
- runtime authorization mutation must allow export parsing and staged export preparation immediately after the root is added
- runtime authorization mutation must allow media inventory collection immediately after the root is added
- runtime authorization mutation must allow audio input validation to progress beyond authorization checks once the input root is added
- runtime authorization mutation must allow audio output validation to progress beyond authorization checks once the output root is added
- preparing a workspace with an authorized monitor must preserve both discovered account views and the primary account view
- export-only workspace construction must remain non-empty and preserve discovered account views
- media-only workspace construction must remain non-empty and preserve discovered account views
- preparing a workspace without a watch directory must keep both `watch_dir` and monitor state absent
- audio transcoding must reject unauthorized input assets as well as unauthorized output directories
- account models must fall back to directory names when labels are blank
- account root-path accessors must remain aligned with the canonical data directory
- audio helpers must reject missing input files and report missing `ffmpeg`
- media helpers must classify unknown extensions as `Unknown` and ignore directories
- monitor helpers must ignore unrelated assets and treat an empty pattern list as match-all
- strict `clippy -D warnings` passes on each crate
- current regression counts:
  - WeChat: `61 passed`
  - each non-WeChat crate in this subset: `61 passed`

The same workspace/account/watch-dir regression layer now also covers the remaining non-WeChat
platforms:

- iMessage (`xenobot-imessage`)
- Messenger (`xenobot-messenger`)
- KakaoTalk (`xenobot-kakaotalk`)
- Slack (`xenobot-slack`)
- Teams (`xenobot-teams`)
- Signal (`xenobot-signal`)
- Skype (`xenobot-skype`)
- Google Chat (`xenobot-googlechat`)
- Zoom (`xenobot-zoom`)
- Viber (`xenobot-viber`)

For these platforms, Xenobot now also enforces:

- runtime authorization mutation must allow export parsing immediately after the root is added
- runtime authorization mutation must allow staged export preparation immediately after the root is added
- runtime authorization mutation must allow media inventory collection immediately after the root is added
- runtime authorization mutation must allow audio output validation to progress beyond authorization checks once the output root is added
- preparing a workspace with an authorized monitor must preserve both discovered account views and the primary account view
- export-only workspace construction must remain non-empty and preserve discovered account views
- media-only workspace construction must remain non-empty and preserve discovered account views
- preparing a workspace without a watch directory must keep both `watch_dir` and monitor state absent
- account models must fall back to directory names when labels are blank
- account root-path accessors must remain aligned with the canonical data directory
- audio helpers must reject missing input files and report missing `ffmpeg`
- media helpers must classify unknown extensions as `Unknown` and ignore directories
- monitor helpers must ignore unrelated assets and treat an empty pattern list as match-all

Current regression counts are now:

- each crate: `61 passed`

All platform crates now additionally satisfy the current strict quality gate:

- `scripts/check_platform_coverage.sh` passes end-to-end
- all 17 platform crates pass `cargo clippy --all-targets --offline -- -D warnings`

## Remaining Gap To Full WeChat Parity

The platform-depth block is not considered fully complete yet.

What still remains is not general platform-service depth. That parity layer is now in place.

The only remaining WeChat-specific gap is the decrypt/runtime detector path:

- WeChat keeps additional legal-safe decrypt boundary coverage:
  - unauthorized decrypt input paths fail before key lookup
  - unauthorized derived output paths fail before file creation
  - PID-scoped decrypt output directories are created eagerly and decryption failures emit completion events
  - core V4 decrypt helpers are covered by stable reference-vector and failure-path tests
  - service-level detector/runtime cache/event regressions sit on top of the decrypt boundary checks
- non-WeChat crates intentionally do not copy this behavior mechanically, because that would stop reflecting their own platform semantics
- current WeChat regression count: `61 passed`
- current non-WeChat regression count: `61 passed`

`decrypt` is intentionally treated differently:

- Xenobot does **not** implement process-memory extraction
- Xenobot does **not** implement encryption bypass
- any future decryption-related work must remain within the legal-safe, user-authorized export model

## Validation

Run:

```bash
scripts/check_platform_coverage.sh
```

The script verifies:

- `xenobot-core` platform discovery tests.
- each platform adapter crate test set.
- API + CLI compile contract (`api,analysis` feature set).
- agent frontend/backend alias contract regression (`7.2`).
- MCP protocol/tool/resource contract regression (`10.x`).
