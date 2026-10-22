# Strict Invariant: No Lifecycle Mutations While Managed KVMs Are Running

To eliminate complex runtime ACPI race conditions, filesystem corruption risks, and unpredictable state transitions, `onehost apply` enforces a strict hypervisor quiescence invariant. If any onehost-managed domain is currently running or paused, `onehost apply` aborts immediately during pre-flight validation before executing any disk or hypervisor mutations. Operators must cleanly shut down active domains before applying infrastructure changes.
