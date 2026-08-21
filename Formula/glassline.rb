class Glassline < Formula
  desc "Rust port of ccstatusline — status line formatter for Claude Code CLI"
  homepage "https://github.com/kurtbot/glassline"
  version "0.6.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/kurtbot/glassline/releases/download/v0.6.0/glassline-aarch64-apple-darwin.tar.gz"
      sha256 "35bce08a3a5b22a9a0fafc42a144dd604cf43b88a8b956ca970630ce5f1648da"
    end
    on_intel do
      url "https://github.com/kurtbot/glassline/releases/download/v0.6.0/glassline-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_MAC_X64"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/kurtbot/glassline/releases/download/v0.6.0/glassline-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "69b06458ed6e719c3b73a3b621921e703ecb0da67c237b579e7b9655e444420b"
    end
    on_intel do
      url "https://github.com/kurtbot/glassline/releases/download/v0.6.0/glassline-x86_64-unknown-linux-gnu.tar.gz"
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
