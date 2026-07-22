require "rails_helper"

RSpec.describe "Application data configuration" do
  it "prepares an isolated test data root during boot" do
    application_data = Rails.configuration.x.librairii.application_data

    expect(application_data.root).to eq(Rails.root.join("tmp/librairii/test"))
    expect(Rails.configuration.x.librairii.data_root).to eq(application_data.root)
    expect(application_data.root).to be_directory
    expect(application_data.path("db")).to be_directory
  end

  it "stores the test SQLite database below the data root" do
    database = ActiveRecord::Base.configurations.configs_for(
      env_name: "test",
      name: "primary"
    ).database

    expected_database = Rails.configuration.x.librairii.application_data
      .path("db")
      .join("test.sqlite3")

    expect(Pathname(database)).to eq(expected_database)
  end
end
