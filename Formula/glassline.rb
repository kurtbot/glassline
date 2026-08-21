class Glassline < Formula
  desc "Rust port of ccstatusline — status line formatter for Claude Code CLI"
  homepage "https://github.com/kurtbot/glassline"
  version "0.6.2"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/kurtbot/glassline/releases/download/v0.6.2/glassline-aarch64-apple-darwin.tar.gz"
      sha256 "3d02d97be2652c140ff7ee78c02e1668132ea733a253669419c4a4f10d7547c2"
    end
    on_intel do
      url "https://github.com/kurtbot/glassline/releases/download/v0.6.2/glassline-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_MAC_X64"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/kurtbot/glassline/releases/download/v0.6.2/glassline-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "bb2301a2571a8e2dd8673402150e6933c48e47fef8914a499f4d43d75d02d02d"
    end
    on_intel do
      url "https://github.com/kurtbot/glassline/releases/download/v0.6.2/glassline-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER_LINUX_X64"
    end
  end

  def install
    bin.install "glassline"
    # Ship the interactive editor next to the render binary so the TTY
    # shim (bare `glassline` in a terminal) can exec its sibling. The
    # release archive includes both since v0.6.2.
    bin.install "glassline-tui" if File.exist?("glassline-tui")
  end

  test do
    assert_match "glassline #{version}", shell_output("#{bin}/glassline --version")
  end
end
