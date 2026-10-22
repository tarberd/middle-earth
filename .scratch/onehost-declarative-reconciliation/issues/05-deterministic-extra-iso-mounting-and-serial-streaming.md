# 05: Deterministic Extra ISO Mounting and Sanitized Guest Serial Streaming

**What to build:** Enhances the headless QEMU image builder to support arbitrary drivers and real-time setup monitoring. The builder attaches all extra ISOs declared in the flavor in deterministic alphabetical order as read-only CD-ROM drives alongside the OEMDRV media and Windows installation ISO. Guest serial output (`COM1`) is streamed in real time to stderr with a `[guest-serial]` prefix, and terminal reset escape sequences are stripped so host terminal displays remain uncorrupted.

**Blocked by:** 01: Manifest Schema Simplification, Canonical Release Tuple, and Dynamic Content Hashing

**Status:** ready-for-agent

- [ ] Extra ISOs declared in the flavor are mounted in headless QEMU as read-only CD-ROM drives ordered alphabetically by key.
- [ ] Headless QEMU launches with Windows ISO, OEMDRV ISO, and extra ISOs properly configured on distinct CD-ROM drives.
- [ ] Guest serial console output (`COM1`) streams to stderr in real time prefixed with `[guest-serial]`.
- [ ] Terminal reset escape codes (such as `\x1b[2J` or `\x1bc`) in guest serial stream are sanitized prior to printing.
