require "rails_helper"

RSpec.describe "Authenticated desktop launch", type: :system do
  before do
    driven_by :rack_test
  end

  it "opens the collection from the authenticated launch URL" do
    visit "/?launch_secret=#{Rails.configuration.x.librairii.launch_secret}"

    expect(page).to have_current_path("/")
    expect(page).to have_content("Your local story collection is ready.")

    visit "/"

    expect(page).to have_content("Your local story collection is ready.")
  end

  it "rejects a browser without the per-launch secret" do
    visit "/"

    expect(page.status_code).to eq(403)
    expect(page).to have_content("Local launch authentication required.")
  end
end
