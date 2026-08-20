class Glassline < Formula
  desc "Rust port of ccstatusline — status line formatter for Claude Code CLI"
  homepage "https://github.com/kurtbot/glassline"
  version "0.5.1"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/kurtbot/glassline/releases/download/v0.5.1/glassline-aarch64-apple-darwin.tar.gz"
      sha256 "aaacc4d8b0f7ff46b94db04662f5ec24b4ae8c58ee3e79e847b79bd5a09d63a3"
    end
    on_intel do
      url "https://github.com/kurtbot/glassline/releases/download/v0.5.1/glassline-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_MAC_X64"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/kurtbot/glassline/releases/download/v0.5.1/glassline-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "5d1060e7bee491f6038931c4030d865ac33d0b651a3f2a5331250fc8579092e1"
    end
    on_intel do
      url "https://github.com/kurtbot/glassline/releases/download/v0.5.1/glassline-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER_LINUX_X64"
    end
  end

  def install
    bin.install "glassline"
  end

  test do
    assert_match "glassline #{version}", shell_output("#{bin}/glassline --version")
  end
end
