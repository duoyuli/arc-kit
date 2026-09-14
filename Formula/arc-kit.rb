class ArcKit < Formula
  desc "CLI tool for managing coding agent providers, skills, and markets"
  homepage "https://github.com/duoyuli/arc-kit"
  license "MIT"
  version "2026.9.14"

  on_arm do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.9.14/arc-kit-aarch64-apple-darwin.tar.gz"
    sha256 "45e533692868e8ea3d35f626cf70845d08dd6c20b6427c666bcdf86dcf4c9f9a"
  end

  on_intel do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.9.14/arc-kit-x86_64-apple-darwin.tar.gz"
    sha256 "b3bd230073d2ff2502409d0e0708e23adec6906877fa76992ada81030cf9c55d"
  end

  def install
    bin.install "arc"
  end

  test do
    system bin/"arc", "version"
  end
end
