class Glassline < Formula
  desc "Rust port of ccstatusline — status line formatter for Claude Code CLI"
  homepage "https://github.com/kurtbot/glassline"
  version "0.7.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/kurtbot/glassline/releases/download/v0.7.0/glassline-aarch64-apple-darwin.tar.gz"
      sha256 "54bff9e332faf6c1f18e2a5e8426d2800159aa3341a4a3ff73ab15d32ac84939"
    end
    on_intel do
      url "https://github.com/kurtbot/glassline/releases/download/v0.7.0/glassline-x86_64-apple-darwin.tar.gz"
      sha256 "658018db1ed505c639b5901308dc7af375f327813e2186fd23d6668f7c121389"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/kurtbot/glassline/releases/download/v0.7.0/glassline-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "9a5dfa2ff9eb376e481aec6c927720dbbf2ca8dc6f205756f74bbb87cd41a3f0"
    end
    on_intel do
      url "https://github.com/kurtbot/glassline/releases/download/v0.7.0/glassline-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "00017c399ae4f45a7d6f8736da914eca2f87d0f7ac6c76b97e56cd1b20a83975"
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
