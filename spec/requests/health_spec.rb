require "rails_helper"

RSpec.describe "Health", type: :request do
  it "accepts an authenticated loopback probe" do
    get health_path, headers: launch_authorization

    expect(response).to have_http_status(:ok)
    expect(response.parsed_body).to eq("status" => "ok")
  end

  it "rejects a probe without the launch secret" do
    get health_path

    expect(response).to have_http_status(:forbidden)
    expect(response.body).to eq("Local launch authentication required.")
  end

  it "rejects an authenticated probe from a non-loopback address" do
    get health_path, headers: launch_authorization.merge("REMOTE_ADDR" => "203.0.113.10")

    expect(response).to have_http_status(:forbidden)
  end

  it "reports storage readiness failures" do
    issue = Librairii::Readiness::Issue.new(code: :database, message: "Database is unavailable.")
    result = Librairii::Readiness::Result.new(issues: [ issue ])
    readiness = Class.new do
      define_method(:call) { result }
    end.new
    original_readiness = Rails.configuration.x.librairii.readiness
    Rails.configuration.x.librairii.readiness = readiness

    get health_path, headers: launch_authorization

    expect(response).to have_http_status(:service_unavailable)
    expect(response.parsed_body).to eq("status" => "not_ready", "issues" => [ "database" ])
  ensure
    Rails.configuration.x.librairii.readiness = original_readiness
  end
end
