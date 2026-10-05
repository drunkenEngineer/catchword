# Catchword documentation

- `adr/` holds Architecture Decision Records, one file per decision.
- `user/` is for people who use Catchword: the [user guide](user/guide.md) (a draft), [what leaves your computer](user/network.md) with steps to check it, and the [privacy policy](user/privacy-policy.md) the Microsoft Store asks for (a draft, missing a contact address).
- `threat-model.md` takes the threat model of the specification (section 13) threat by threat: what is in place and where, what is not yet, and the review checklist for the guarded areas.
- `packaging.md` covers the installer, the MSIX package and the licence notices.
- `benchmarks/` holds dated measurements against section 14.
- `specification.md` is the full product and architecture specification (26 sections). Split it into smaller files during Phase 0.

## Before the first public push

- Add a contact address to `CODE_OF_CONDUCT.md`, and to `user/privacy-policy.md`.
- Confirm the name: domain, trademark search, Microsoft Store reservation.
