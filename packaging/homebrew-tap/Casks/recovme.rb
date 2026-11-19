cask "recovme" do
  version "0.3.3"
  sha256 "42b31a4db4cc997903a55b95bca200281cc573fe61966064cfccba65c88e90bb"

  url "https://github.com/trevnedai/recovme/releases/download/v#{version}/RecovMe_#{version}_aarch64.dmg",
      verified: "github.com/trevnedai/recovme/"
  name "RecovMe"
  desc "File and photo recovery for cards, drives, phones and backups"
  homepage "https://recovme.app/"

  livecheck do
    url :url
    strategy :github_latest
  end

  auto_updates true
  depends_on arch: :arm64
  depends_on macos: ">= :big_sur"

  app "RecovMe.app"

  zap trash: [
    "~/Library/Application Support/app.recovme.recovery",
    "~/Library/Caches/app.recovme.recovery",
    "~/Library/Preferences/app.recovme.recovery.plist",
    "~/Library/Saved Application State/app.recovme.recovery.savedState",
    "~/Library/WebKit/app.recovme.recovery",
  ]
end
