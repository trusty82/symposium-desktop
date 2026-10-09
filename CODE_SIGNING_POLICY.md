# Code signing policy

Free code signing provided by [SignPath.io](https://signpath.io), certificate by
[SignPath Foundation](https://signpath.org).

This policy applies to **Symposium**, the desktop companion of the World of Warcraft
addon Symposium and of the guild site [symposium-gaming.com](https://symposium-gaming.com),
developed in this repository: <https://github.com/trusty82/symposium-desktop>.

## What is signed

Only the Windows release files of this project:

- the installer `Symposium_X.Y.Z_x64-setup.exe` (NSIS);
- the application executable it installs (`Symposium.exe`).

Nothing built on a developer's computer is ever signed. No third-party program is signed
under this project.

## How releases are built and signed

1. A release starts from a version tag `vX.Y.Z` pushed on the `main` branch of this
   repository.
2. The [Release workflow](.github/workflows/release.yml) builds the application on
   GitHub-hosted runners (GitHub Actions), from this repository only. Dependencies come
   from the public registries (crates.io, npm) and are pinned by `Cargo.lock` and
   `package-lock.json`.
3. The workflow submits the unsigned build to SignPath. **Each signing request is
   reviewed and approved manually** by an approver (see below) before it is signed.
4. Signed files are published on the project's GitHub Releases and on
   [symposium-gaming.com](https://symposium-gaming.com) (download page and automatic
   updates). The automatic updater additionally verifies a separate update signature
   (minisign) embedded in the application.

## Team roles

| Role | Members |
| --- | --- |
| Committers and reviewers | [Rémi Bouille (@trusty82)](https://github.com/trusty82) |
| Approvers | [Rémi Bouille (@trusty82)](https://github.com/trusty82) |

All team members use multi-factor authentication on GitHub and on SignPath. Changes from
outside contributors are only merged after review by a committer.

## Privacy policy

This program will not transfer any information to other networked systems unless
specifically requested by the user or the person installing or operating it.

What it does, once the user has configured it:

- **Character sheet**: when the user pastes their personal token (created on their profile
  at symposium-gaming.com) and selects the addon's `SavedVariables\Symposium.lua` file,
  the application sends the character export written by the addon (name, realm, class,
  level, equipment, statistics, talents, professions) to symposium-gaming.com.
- **Boss fights** (can be turned off in the application): from the game's combat log on the
  user's computer, the application sends a summary of each boss fight (damage, healing and
  deaths of the group members) to symposium-gaming.com. The combat log itself never leaves
  the computer.
- **Updates**: the application checks symposium-gaming.com (and GitHub as a fallback) for a
  new version of itself, and symposium-gaming.com for a new version of the Symposium addon
  (can be turned off). These checks only send the application version and, for the addon,
  the user's token.

The token and the World of Warcraft folder path stay in the application's settings on the
user's computer. Data received by the site is described in its privacy policy:
<https://symposium-gaming.com/confidentialite>.

## Contact

Questions or security reports: open an issue on this repository, or contact the guild
council on the Discord server linked from [symposium-gaming.com](https://symposium-gaming.com).
