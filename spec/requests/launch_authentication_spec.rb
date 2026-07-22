require "rails_helper"

RSpec.describe "Launch authentication", type: :request do
  it "exchanges the launch query secret for an HTTP-only cookie" do
    get root_path, params: {
      launch_secret: Rails.configuration.x.librairii.launch_secret,
      retained: "value"
    }

    expect(response).to redirect_to("/?retained=value")
    expect(response.headers.fetch("set-cookie")).to include("_librairii_launch=")
    expect(response.headers.fetch("set-cookie")).to include("HttpOnly")
    expect(response.headers.fetch("set-cookie")).to include("SameSite=Strict")

    follow_redirect!

    expect(response).to have_http_status(:ok)
    expect(response.body).to include("Your local story collection is ready.")
  end

  it "rejects an invalid bearer secret" do
    get root_path, headers: launch_authorization("wrong-secret")

    expect(response).to have_http_status(:forbidden)
  end
end
