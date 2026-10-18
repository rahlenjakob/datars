# NVDA on a Windows runner — prepared, not run

`nvda.yml` builds the web runtime and the plain test pages (`scripts/build-a11y-pages.sh`) on
Linux, then on `windows-latest` installs NVDA with [Guidepup](https://github.com/guidepup/guidepup)
(`@guidepup/setup`, MIT) and drives it over each page with `@guidepup/guidepup` (MIT), saving what
NVDA said as the `nvda-said` artifact. Guidepup is installed inside the job with `npx` / `npm
install --no-save`: it is not a dependency of any datars package.

It has **never run**. It is kept outside `.github/workflows/` so it doesn't run on push. To run it:

1. Copy `ci/windows-nvda/nvda.yml` to `.github/workflows/nvda.yml` on a branch and push the branch.
2. In the repository's Actions tab, choose **nvda** → **Run workflow** on that branch.
3. Download the `nvda-said` artifact: one `<chart>.txt` per page (each step and NVDA's last phrase)
   and `<chart>.log.txt` (everything NVDA spoke).

JAWS is not covered: it needs a licence, and Guidepup doesn't drive it.
