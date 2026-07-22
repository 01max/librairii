require "rails_helper"

RSpec.describe "Library", type: :request do
  describe "GET /" do
    it "renders the local collection root" do
      get root_path

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("Librairii")
      expect(response.body).to include("Your local story collection is ready.")
    end
  end
end
