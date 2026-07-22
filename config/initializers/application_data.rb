require Rails.root.join("lib/librairii/application_data")

application_data = Librairii::ApplicationData.resolve(
  environment: Rails.env,
  application_root: Rails.root,
  configured_root: Rails.configuration.x.librairii.data_root
)

preparation_error = begin
  application_data.prepare!
  nil
rescue SystemCallError => error
  error
end

Rails.configuration.x.librairii.data_root = application_data.root
Rails.configuration.x.librairii.application_data = application_data
Rails.configuration.x.librairii.application_data_preparation_error = preparation_error
