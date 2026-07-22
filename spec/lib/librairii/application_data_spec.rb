require "rails_helper"

RSpec.describe Librairii::ApplicationData do
  describe ".resolve" do
    it "isolates the default root by environment" do
      development = described_class.resolve(
        environment: :development,
        application_root: Rails.root
      )
      test = described_class.resolve(
        environment: :test,
        application_root: Rails.root
      )

      expect(development.root).to eq(Rails.root.join("tmp/librairii/development"))
      expect(test.root).to eq(Rails.root.join("tmp/librairii/test"))
      expect(development.root).not_to eq(test.root)
    end

    it "uses an explicitly configured root" do
      configured_root = Rails.root.join("tmp/explicit-data-root")

      application_data = described_class.resolve(
        environment: :test,
        application_root: Rails.root,
        configured_root: configured_root.to_s
      )

      expect(application_data.root).to eq(configured_root)
    end
  end

  describe "#prepare!" do
    it "creates every application data directory" do
      Dir.mktmpdir("librairii-data") do |temporary_root|
        application_data = described_class.new(temporary_root).prepare!

        described_class::DIRECTORY_NAMES.each do |name|
          expect(application_data.path(name)).to be_directory
        end
      end
    end

    it "rejects paths outside the declared layout" do
      application_data = described_class.new(Rails.root.join("tmp/librairii/test"))

      expect { application_data.path("unknown") }
        .to raise_error(ArgumentError, /unknown application data directory/)
    end
  end
end
