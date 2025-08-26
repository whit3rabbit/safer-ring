---
name: Bug report
about: Create a report to help us improve safer-ring
title: ''
labels: 'bug'
assignees: ''
---

**Describe the bug**
A clear and concise description of what the bug is.

**To Reproduce**
Steps to reproduce the behavior:
1. Create a minimal reproduction case
2. Run with specific commands
3. See error

```rust
// Please provide a minimal code example that demonstrates the issue
use safer_ring::*;

fn main() {
    // Your reproduction code here
}
```

**Expected behavior**
A clear and concise description of what you expected to happen.

**Actual behavior**
What actually happened instead.

**Environment:**
 - OS and version: [e.g. Ubuntu 22.04, macOS 14.0]
 - Kernel version (if Linux): [e.g. 6.2.0]
 - Rust version: [e.g. 1.75.0]
 - safer-ring version: [e.g. 0.1.0]

**Additional context**
Add any other context about the problem here.

**Safety Impact**
- [ ] This might be a memory safety issue
- [ ] This could lead to use-after-free
- [ ] This involves undefined behavior
- [ ] This is a performance issue only
- [ ] This is a usability/API issue only