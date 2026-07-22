require "rails_helper"

RSpec.describe "Storage recovery", type: :request do
  let(:readiness) do
    Class.new do
      def initialize(result)
        @result = result
      end

      def call
        @result
      end
    end.new(readiness_result)
  end
  let(:issue) do
    Librairii::Readiness::Issue.new(
      code: :data_root,
      message: "The application data folder is not writable."
    )
  end
  let(:readiness_result) { Librairii::Readiness::Result.new(issues: [ issue ]) }

  around do |example|
    original_readiness = Rails.configuration.x.librairii.readiness
    Rails.configuration.x.librairii.readiness = readiness
    example.run
  ensure
    Rails.configuration.x.librairii.readiness = original_readiness
  end

  it "redirects collection reads to the recovery page" do
    get root_path

    expect(response).to redirect_to(recovery_path)

    follow_redirect!

    expect(response).to have_http_status(:service_unavailable)
    expect(response.body).to include("Librairii needs attention")
    expect(response.body).to include("No changes can be made")
  end

  it "blocks mutation requests before routing them" do
    post "/a-future-mutation"

    expect(response).to have_http_status(:service_unavailable)
    expect(response.body).to include("Librairii needs attention")
  end

  context "when storage recovers" do
    let(:readiness_result) { Librairii::Readiness::Result.new(issues: []) }

    it "allows the collection to load" do
      get root_path

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("Your local story collection is ready.")
    end
  end
end
