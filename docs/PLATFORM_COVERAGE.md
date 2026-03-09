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
