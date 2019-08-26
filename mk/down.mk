#####################
# Stopping commands #
#####################

down.demo: docker.down.demo

# Stop all related to Medea services.

down:
	@make down.medea dockerized=yes
	@make down.medea dockerized=no
	@make down.coturn


# Stop Medea media server.
#
# Defaults:
# 	dockerized=no
#
# Usage:
# 	make down.medea [dockerized=(yes|no)]

down.medea:
ifeq ($(dockerized),yes)
	docker-compose -f docker-compose.medea.yml down
else
	- killall medea
endif


# Stop all services needed for e2e testing of medea in browsers.
#
# Usage:
#   make down.e2e.services [dockerized=(yes|no)] [coturn=(yes|no)]

down.e2e.services:
ifeq ($(dockerized),no)
	kill $$(cat /tmp/e2e_medea.pid)
	kill $$(cat /tmp/e2e_control_api_mock.pid)
	rm -f /tmp/e2e_medea.pid \
		/tmp/e2e_control_api_mock.pid
ifneq ($(coturn),no)
	@make down.coturn
endif
else
	docker container stop $$(cat /tmp/control-api-mock.docker.uid)
	docker container stop $$(cat /tmp/medea.docker.uid)
	rm -f /tmp/control-api-mock.docker.uid /tmp/medea.docker.uid

	@make down.coturn
endif


# Stop dockerized coturn.
#
# Usage:
# 	make down.coturn

down.coturn:
	docker-compose -f docker-compose.coturn.yml down
