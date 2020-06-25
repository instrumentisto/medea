while system('make test.e2e up=yes dockerized=no')
	puts "Tests passed"
end
