# GOALS.md

> **Project:** Lumo File Explorer  
> **Vision:** The fastest, most private, developer-friendly file manager that works seamlessly across Windows and Linux

---

## Performance Goals (Hard Targets)

These are non-negotiable metrics that must be achieved before 1.0 release:

| Metric | Target | Measurement Method | Priority |
|--------|--------|-------------------|----------|
| **Cold Start Time** | <500ms on SSD | `time lumo --version` (10x average) | Critical |
| **Folder Load (1K files)** | <200ms | Benchmark suite | Critical |
| **Folder Load (10K files)** | <800ms | Benchmark suite | Critical |
| **Folder Load (100K files)** | <2s | Streaming mode | High |
| **Tab Switch Latency** | <50ms | Profiler timing | Critical |
| **Search Response (indexed)** | <50ms | Benchmark suite | High |
| **Search Response (real-time)** | <500ms | Ripgrep integration | High |
| **Memory Usage (Idle)** | <50MB | Heap profiler | Critical |
| **Memory Usage (Active)** | <200MB | Heap profiler | High |
| **Memory Usage (Large Dir)** | <500MB | Stress test (1M files) | High |
| **UI Frame Rate** | 60fps locked | GPU profiler | Critical |
| **Thumbnail Generation** | <100ms per image | Batch test | High |
| **Preview Load (100MB image)** | <500ms | Benchmark | Medium |
| **Batch Rename (1K files)** | <5s | Operational timing | High |
| **Sync/Transfer Resume** | Instant | Interrupt/resume test | Medium |
| **AI Rename (100 files, local)** | <10s | Offline LLM | Medium |

---

## Core Features (Priority: Critical)

Essential functionality required for daily use. Without these, the app is unusable.

### 1. Filesystem Navigation
| Feature | Description | Target |
|---------|-------------|--------|
| Directory Browsing | Navigate folders with keyboard/mouse | Standard |
| Path Display | Full path in address bar | Standard |
| Back/Forward Stack | Maintain navigation history | Standard |
| Favorites/Bookmarks | Pin frequently used folders | Standard |
| Recent Files | Automatic recency tracking | Standard |
| Search Box | Filter files by name as you type | <100ms response |
| Go-to Input | Jump to arbitrary path (Ctrl+G) | Instant |
| Parent Directory Access | Up one level (Ctrl+Up) | Instant |

### 2. File Operations
| Feature | Description | Target |
|---------|-------------|--------|
| Copy/Move/Delete | Standard file operations | Atomic, recoverable |
| Rename | Single file rename | Instant |
| Create Folder/File | New folder/file from UI | Instant |
| Trash/Recycle Bin | Safe deletion with restore | Full integration |
| Drag & Drop | Move/copy between panes/windows | OLE-compliant (Windows) |
| Undo/Redo | Reverse recent operations | Unlimited depth |
| Permissions Viewer | Show/edit file permissions | Full support |
| Properties Dialog | Detailed metadata display | Standard |

### 3. Window Management
| Feature | Description | Target |
|---------|-------------|--------|
| Tabs | Multiple folders in one window | <50ms switch |
| Tab Persistence | Restore tabs after close | 100% recovery |
| Multi-window | Spawn additional windows | Standard |
| Split Panes | Dual-pane view | Optional but ready |
| Fullscreen Mode | Hide UI chrome | Standard |
| Window State | Remember size/position | Standard |

### 4. Shell Integration
| Feature | Description | Platform |
|---------|-------------|----------|
| Win+E Override | Intercept keyboard shortcut | Windows only |
| System Folder Hook | Downloads/Documents redirect | Windows only |
| Context Menu Entry | Right-click → Open with Lumo | Both |
| Default File Manager | Set as system default | Both |
| MIME Handler | `inode/directory` association | Linux only |
| Desktop Integration | .desktop file registration | Linux only |
| Uninstall Cleanup | Remove all hooks cleanly | Both |

---

## Essential Features (Priority: High)

Features that significantly improve productivity. Should be present at launch.

### 5. Search & Filtering
| Feature | Description | Target |
|---------|-------------|--------|
| Filename Search | Filter as you type | <100ms |
| Content Search | Search inside files (text) | <500ms |
| Regex Support | Advanced pattern matching | Full PCRE |
| Filters | By type, date, size, tag | Instant |
| Saved Searches | Reuse common queries | Standard |
| Highlight Matches | Visual emphasis on results | Standard |
| Case Sensitivity Toggle | Match options | Standard |
| Hidden Files Toggle | Show/hide dotfiles | Standard |

### 6. Preview System
| Feature | Description | Target |
|---------|-------------|--------|
| Image Preview | PNG, JPG, WEBP, GIF, SVG | <100ms load |
| Text Preview | Any text file encoding | Instant |
| Code Preview | Syntax highlighting (20+ langs) | <500ms |
| Markdown Preview | Render MD with formatting | <500ms |
| PDF Preview | Render first page | <2s |
| Media Playback | MP4, MP3, AVI in preview | 60fps |
| Thumbnail Cache | Disk-persistent thumbnails | Auto-evict |
| Quick Look / Quick Peek | Press Space for a floating preview of the selected file | <50ms trigger |

### 7. Developer Tools
| Feature | Description | Target |
|---------|-------------|--------|
| Inline Terminal | Embedded shell (PowerShell, WSL, bash) | <100ms launch |
| Git Status | Modified/untracked/staged indicators | <500ms refresh |
| Git Diff | Side-by-side file comparison | <2s for 10MB |
| Git History | Log viewer per file | Standard |
| Syntax Previews | Colorized code without IDE | 20+ languages |
| Simple Text Editor | Open, edit and save text/code files in-app (syntax highlighting, find, undo) — not an IDE | Instant open |
| File Watcher | Detect changes externally | Real-time |
| SSH/SFTP Mount | Remote folders as local | <5s connect |
| Docker Container Access | Browse container filesystems | Standard |

### 8. Batch Operations
| Feature | Description | Target |
|---------|-------------|--------|
| Bulk Rename | Regex, templates, sequences | <5s for 1K files |
| Duplicate Finder | Hash + fuzzy matching | <10min for 100GB |
| Copy Queue | Manage multiple transfers | Pause/resume |
| Sync Folders | Mirror directories | <2min for 5K files |
| Extract Archives | ZIP, RAR, 7Z, TAR | Native speeds |
| Space-Saving Extraction | Extract with [HatchZip](https://sourceforge.net/mirror/hatchzip/), freeing the archive's space as it goes — for huge archives on full disks | Optional extract mode |
| Compress Files | Create archives | Native speeds |
| Checksum Calc | MD5, SHA256, CRC32 | Concurrent |

### 9. User Interface
| Feature | Description | Target |
|---------|-------------|--------|
| Light/Dark Themes | System-aware toggles | Instant switch |
| Custom Themes | Edit colors/layouts | JSON config |
| Keyboard Shortcuts | Customizable bindings | Full remap |
| Command Palette | Fuzzy search over commands, tabs, places, files (Ctrl+K / Ctrl+P); every new feature (Git, AI, archives, editor…) adds its commands | <100ms ✅ |
| Context Menu | Right-click menu for files, folders, selection and empty space | Standard |
| Responsive Layout | Adapt to window size | Smooth |
| Accessibility | Screen reader support | WCAG 2.1 AA |
| Localization | 10+ languages at launch | Standard |
| Tooltips/Hints | Discoverability aids | Standard |

### 10. Privacy & Security
| Feature | Description | Target |
|---------|-------------|--------|
| No Telemetry by Default | Nothing collected unless the user opts in (crash reports, usage stats) | Hard constraint |
| Offline AI | All AI runs locally | Mandatory default |
| Sandboxed Plugins | Isolated extension execution | Memory-safe |
| Path Validation | Prevent traversal attacks | All inputs |
| Secure Storage | Encrypt API keys/configs | AES-256 |
| AppArmor/SELinux | Linux security profiles | Optional |
| Firewall Compliance | Block outbound by default | Standard |

---

## Advanced Features (Priority: Medium)

Nice-to-have features that enhance power-user workflows. Can be added post-launch or as plugins.

### 11. AI-Powered Intelligence
| Feature | Description | Target |
|---------|-------------|--------|
| Semantic Search | Natural language queries | <2s offline |
| AI Rename Suggestions | Context-aware naming | <10s for 100 files |
| Auto-Organization | Recommend folder structure | <1min analysis |
| Chat with Files | RAG-based Q&A | <5s per answer |
| Content Summaries | AI-generated file abstracts | <5s per doc |
| Duplicate Detection | Fuzzy content matching | <10min for 100GB |
| Smart Tags | Auto-tag based on content | Background |
| File Recommendations | "You might want this" suggestions | Context-aware |

### 12. Cloud & Remote
| Feature | Description | Target |
|---------|-------------|--------|
| Google Drive Mount | OAuth integration | <5s connect |
| OneDrive Mount | Microsoft 365 integration | <5s connect |
| Dropbox Mount | API sync | <5s connect |
| S3/Blob Mount | AWS/Azure storage | <5s connect |
| WebDAV Client | Generic WebDAV support | Standard |
| FTP/SFTP Client | Legacy protocol support | Standard |
| Conflict Resolution | Handle simultaneous edits | Merge prompts |
| Offline Cache | Work disconnected | Auto-sync later |

### 13. Collaboration
| Feature | Description | Target |
|---------|-------------|--------|
| Shared Workspaces | Team folder views | Multi-user |
| Comment Threads | Annotate files | In-app |
| Version History | Track changes over time | Git-integrated |
| File Requests | Collect uploads from others | Web form |
| Activity Feed | Recent changes timeline | Real-time |
| Permission Sharing | Grant folder access | Role-based |

### 14. Visualization
| Feature | Description | Target |
|---------|-------------|--------|
| Disk Usage Map | Treemap of folder sizes | <5s render |
| File Type Distribution | Pie/bar charts | Instant |
| Timeline View | Files by modification date | Interactive |
| Folder Comparison | Side-by-side diff | <2s |
| File Link Graph | Show relationships | Experimental |
| Heatmaps | Access frequency visualization | Optional |

### 15. Automation
| Feature | Description | Target |
|---------|-------------|--------|
| Custom Scripts | Run external tools on events | Bash/PowerShell |
| Hotkeys | Macro recording | Standard |
| Event Triggers | On file add/remove | Subsecond |
| Watched Folders | Auto-actions on change | Real-time |
| Workflow Templates | Preset automation chains | Import/export |
| CLI Integration | Headless mode for scripting | Stdout/stderr |

---

## Optional Features (Priority: Low)

Future enhancements or niche capabilities. Will be built as plugins if community demand exists.

### 16. Plugin Ecosystem
| Feature | Description | Implementation |
|---------|-------------|----------------|
| Plugin SDK | Rust + WASM API | Post-1.0 |
| Marketplace | Curated plugin repository | Community-hosted |
| Hot-Reload Plugins | Update without restart | Advanced |
| Plugin Sandbox | Restricted permissions | Security audit |
| Plugin Updates | Automatic check/notify | Background |
| Community Contributions | Submit/share plugins | GitHub-based |

### 17. Mobile Companion
| Feature | Description | Priority |
|---------|-------------|----------|
| iOS App | Browse remote folders | Future |
| Android App | Full file management | Future |
| Wireless Transfer | Phone ↔ Desktop | Future |
| QR Code Scan | Quick remote access | Future |
| Camera Uploads | Auto-backup photos | Future |
| Offline Mode | Sync when connected | Future |

### 18. Enterprise Features
| Feature | Description | Priority |
|---------|-------------|----------|
| Active Directory | LDAP/AD integration | Future |
| SSO Login | SAML/OIDC auth | Future |
| Audit Logging | Track all file operations | Compliance |
| Policy Enforcement | Admin-controlled rules | Enterprise |
| Group Policies | Deploy via GPO/MDM | Enterprise |
| Encryption at Rest | Per-file encryption | Future |
| Compliance Reports | GDPR/SOC2 export | Enterprise |

### 19. Experimental Ideas
| Feature | Description | Status |
|---------|-------------|--------|
| Neural Search | AI-powered semantic indexing | Research |
| VR/AR Mode | 3D file navigation | Concept |
| Voice Commands | Speech-to-action | Prototype |
| Gesture Control | Touchpad/macros | Niche |
| Blockchain Links | File provenance tracking | Experimental |
| Quantum-safe Crypto | Post-quantum encryption | Research |
| Holographic UI | 3D interface layer | Science fiction |

---

## Non-Functional Requirements

### Compatibility Matrix
| Platform | Minimum Version | Target Version | Support Level |
|----------|-----------------|----------------|---------------|
| Windows 11 | 21H2 | 24H2 | Full |
| Windows 10 | 1809 | 22H2 | Full |
| Ubuntu | 22.04 LTS | 24.04 LTS | Full |
| Fedora | 38 | 40 | Full |
| Arch Linux | Rolling | Latest | Full |
| macOS | 14 (Sonoma) | 15 (Sequoia) | Future |

### Documentation Requirements
| Document | Audience | Target |
|----------|----------|--------|
| User Guide | End users | Complete + searchable |
| Developer Guide | Contributors | API reference + tutorials |
| Architecture Docs | Maintainers | Diagrams + decisions |
| Contributing Guide | Open source | Clear workflow |
| Changelog | All users | Semantic versioning |
| Migration Guide | Upgraders | Breaking changes noted |

### Quality Assurance
| Area | Requirement |
|------|-------------|
| Test Coverage | 90%+ unit tests, 80%+ branches |
| CI Pipeline | Auto-test on every PR |
| Security Audit | Annual third-party review |
| Performance Regression | Blocked if >5% slowdown |
| Accessibility Audit | WCAG 2.1 AA compliant |
| Crash Rate | <0.1% sessions |
| Bug Fix SLA | Critical <7 days, Major <30 days |

---

## Success Metrics (Post-Launch)

Track these KPIs after public release. Metrics that need data from users' machines come **only from opt-in reporting, off by default** — so they measure the opted-in subset, not everyone.

| Metric | Target | Measurement |
|--------|--------|-------------|
| **Daily Active Users** | 10,000+ DAU | Usage stats (opt-in) |
| **Crash-Free Sessions** | 99.9% | Crash reporter (opt-in) |
| **Startup Complaints** | <5% of feedback | User surveys |
| **Adoption Rate** | 1,000+/week | Download stats |
| **Plugin Ecosystem** | 50+ plugins | Marketplace count |
| **Community Contributions** | 100+ PRs/month | GitHub stats |
| **Star Rating** | 4.5+/5 | App stores/reviews |
| **Retention (30-day)** | >70% | Usage stats (opt-in) |

---

## Feature Prioritization Philosophy

### Decision Framework
When evaluating new feature requests, score against:

1. **Impact** — How many users benefit? (1-10)
2. **Effort** — Development complexity (1-10, lower=better)
3. **Differentiation** — Sets us apart from competitors? (Y/N)
4. **Maintenance Cost** — Long-term upkeep burden (1-10)
5. **Privacy Alignment** — Compatible with offline-first vision? (Y/N)

**Priority Score** = Impact ÷ Effort × Differentiation Bonus

Features scoring >1.5 with privacy alignment = **Greenlight**  
Features scoring <0.5 = **Defer to plugins**  
Otherwise = **Evaluate case-by-case**

---

## Roadmap Alignment

This goals document maps to the phased development as follows:

| Phase | Primary Goals Addressed |
|-------|------------------------|
| Phase 0-1 | Core navigation, file operations |
| Phase 2-3 | Tabbed UI, design pass + command palette (2.5), file operations + context menu (2.6), then Windows shell integration |
| Phase 4 | Linux port, desktop integration |
| Phase 5 | Search & filtering |
| Phase 6-7 | Developer tools, previews |
| Phase 8 | Batch operations |
| Phase 9 | Local AI integration |
| Phase 10 | Polish, themes, optimization |
| Phase 11-12 | Cloud, plugins (post-launch) |

---

*Last updated: October 28, 2026*  
*Living document — will evolve based on user feedback and technical discoveries*
