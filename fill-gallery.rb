#!/usr/bin/env ruby

gallery = ""

Dir.glob("gallery/*.svg").each do |file|
  if file == "gallery/test.svg" then next end

  title = file
    .sub(/^gallery\//, "")
    .sub(/\.svg$/, "")
    .gsub(/-/, " ").split(" ")
    .map { |word| word.upcase == word ? word : word.capitalize }
    .join(" ")

  gallery += "**#{title}**\n![#{title}](#{file})\n\n"
end

File.open "README.md", "w" do |f|
  f.write File.read("README.md.in").gsub("%gallery%", gallery)
end
