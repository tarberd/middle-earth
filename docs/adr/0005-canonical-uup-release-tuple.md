# Canonical UUP Release Tuple for Windows Image Identification

Onehost eliminates generic OS namespaces (such as `win11`) and informal build alias strings in favor of the canonical trio (`uup_id`, `edition`, `language`) to uniquely identify Windows installation media. Informal version strings are prone to drift and ambiguity, whereas the UUP dump release tuple provides an exact, unforgeable specification of Microsoft update packages. Golden master images and raw ISOs are named and resolved directly via this canonical trio combined with the flavor's content hash.
