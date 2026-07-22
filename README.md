# Librairii

A local story-library companion for Lunii.QT, built with Ruby on Rails, SQLite,
Hotwire, and Tauri.

## Development

The project uses Ruby 4.0.6 and Rails 8.1.3.

```sh
bin/setup
bin/dev
```

Run the RSpec suite with:

```sh
bundle exec rspec
```

Rails keeps development data below `tmp/librairii/development` and test data
below `tmp/librairii/test`. Set `LIBRAIRII_DATA_ROOT` to use an explicit root;
the desktop shell sets it to the operating system's application-data directory.
