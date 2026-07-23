require "open3"
require "rails_helper"
require "tmpdir"

RSpec.describe "Launch authentication configuration" do
  it "disables authentication for direct development without a launch secret" do
    Dir.mktmpdir("librairii-direct-development") do |data_root|
      environment = {
        "LIBRAIRII_DATA_ROOT" => data_root,
        "LIBRAIRII_LAUNCH_SECRET" => nil,
        "RAILS_ENV" => "development"
      }
      assertion = <<~RUBY
        abort "launch authentication remained enabled" if \
          Rails.configuration.x.librairii.launch_authentication_required
      RUBY
      _stdout, stderr, status = Open3.capture3(
        environment,
        Rails.root.join("bin/rails").to_s,
        "runner",
        assertion,
        chdir: Rails.root.to_s
      )

      expect(status).to be_success, stderr
    end
  end

  it "keeps authentication enabled when the desktop supplies a launch secret" do
    Dir.mktmpdir("librairii-desktop-development") do |data_root|
      environment = {
        "LIBRAIRII_DATA_ROOT" => data_root,
        "LIBRAIRII_LAUNCH_SECRET" => "desktop-launch-secret",
        "RAILS_ENV" => "development"
      }
      assertion = <<~RUBY
        abort "launch authentication was disabled" unless \
          Rails.configuration.x.librairii.launch_authentication_required
        abort "launch secret changed" unless \
          Rails.configuration.x.librairii.launch_secret == "desktop-launch-secret"
      RUBY
      _stdout, stderr, status = Open3.capture3(
        environment,
        Rails.root.join("bin/rails").to_s,
        "runner",
        assertion,
        chdir: Rails.root.to_s
      )

      expect(status).to be_success, stderr
    end
  end
end
