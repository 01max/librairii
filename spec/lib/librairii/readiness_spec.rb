require "rails_helper"

RSpec.describe Librairii::Readiness do
  subject(:result) do
    described_class.new(
      application_data: Rails.configuration.x.librairii.application_data,
      preparation_error: preparation_error,
      file_probe: file_probe,
      database_probe: database_probe
    ).call
  end

  let(:preparation_error) { nil }
  let(:file_probe) { -> { true } }
  let(:database_probe) { -> { true } }

  it "is ready when storage and SQLite accept writes" do
    expect(result).to be_ready
    expect(result.issues).to be_empty
  end

  it "reports an application-data preparation failure" do
    preparation_error = Errno::EACCES.new(Rails.root.join("unwritable").to_s)
    readiness = described_class.new(
      application_data: Rails.configuration.x.librairii.application_data,
      preparation_error: preparation_error,
      file_probe: file_probe,
      database_probe: database_probe
    ).call

    expect(readiness).not_to be_ready
    expect(readiness.issues.map(&:code)).to include(:data_root)
  end

  it "reports a failed file write probe" do
    allow(file_probe).to receive(:call).and_raise(Errno::EACCES)

    expect(result).not_to be_ready
    expect(result.issues.map(&:code)).to include(:data_root)
  end

  it "reports a failed database write probe" do
    allow(database_probe).to receive(:call).and_raise(SQLite3::ReadOnlyException)

    expect(result).not_to be_ready
    expect(result.issues.map(&:code)).to include(:database)
  end
end
